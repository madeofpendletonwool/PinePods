use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use yew::platform::spawn_local;
use crate::requests::setting_reqs::call_get_user_info;
use web_sys::console;
use std::borrow::Borrow;
use crate::requests::setting_reqs::{SettingsUser, call_add_user, AddSettingsUserRequest, EditSettingsUserRequest};
use crate::components::gen_funcs::encode_password;
use crate::components::gen_funcs::validate_user_input;
// use crate::gen_components::_ErrorMessageProps::error_message;


#[function_component(UserSettings)]
pub fn user_settings() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let effect_dispatch = dispatch.clone();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let new_username = use_state(|| "".to_string());
    let new_password = use_state(|| "".to_string());
    let email = use_state(|| "".to_string());
    let fullname = use_state(|| "".to_string());
    let admin_status = use_state(|| false);
    let error_message = state.error_message.clone();
    web_sys::console::log_1(&"testlog".into());
    // Define the type of user in the Vec
    let users: UseStateHandle<Vec<SettingsUser>> = use_state(|| Vec::new());

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
                        Err(e) => console::log_1(&format!("Error getting user info: {}", e).into()),
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

    let on_create_submit = {
        let page_state = page_state.clone();
        let fullname = fullname.clone().to_string();
        let new_username = new_username.clone().to_string();
        let email = email.clone().to_string();
        let new_password = new_password.clone();
        let add_user_request = state.add_settings_user_reqeust.clone();
        let error_message_create = error_message.clone();
        let dispatch_wasm = dispatch.clone();
        Callback::from(move |e: MouseEvent| {
            let dispatch = dispatch_wasm.clone();
            let new_username = new_username.clone();
            let new_password = new_password.clone();
            let fullname = fullname.clone();
            let hash_pw = String::new();
            let salt = String::new();
            let email = email.clone();
            let page_state = page_state.clone();
            let error_message_clone = error_message_create.clone();
            e.prevent_default();
            page_state.set(PageState::Hidden);
            // Hash the password and generate a salt
            match validate_user_input(&new_username, &new_password, &email) {
                Ok(_) => {
                    match encode_password(&new_password) {
                        Ok((hash_pw, salt)) => {
                                        // Set the state
                            dispatch.reduce_mut(move |state| {
                                state.add_settings_user_reqeust = Some(AddSettingsUserRequest {
                                    fullname,
                                    new_username,
                                    email,
                                    hash_pw,
                                    salt,
                                });
                            });

                            let add_user_request = add_user_request.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match call_add_user("http://localhost:8040".to_string(), &add_user_request).await {
                                    Ok(success) => {
                                        if success {
                                            console::log_1(&"User added successfully".into());
                                            page_state.set(PageState::Hidden);
                                        } else {
                                            console::log_1(&"Error adding user".into());
                                            page_state.set(PageState::Hidden);
                                            dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error adding user")));

                                        }
                                    }
                                    Err(e) => {
                                        console::log_1(&format!("Error: {}", e).into());
                                        page_state.set(PageState::Hidden);
                                        dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error adding user: {:?}", e)));
                                    }
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
                                <label for="password" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Password"}</label>
                                <input oninput={on_password_change.clone()} type="password" id="password" name="password" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="fullname" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Full Name"}</label>
                                <input oninput={on_fullname_change.clone()} type="text" id="fullname" name="fullname" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="email" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Email"}</label>
                                <input oninput={on_email_change.clone()} type="email" id="email" name="email" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <button type="submit" onclick={on_create_submit} class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Submit"}</button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define the callback functions
    let on_edit_user = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Edit);
        })
    };

    let on_edit_submit = {
        let page_state = page_state.clone();
        let fullname = fullname.clone().to_string();
        let new_username = new_username.clone().to_string();
        let email = email.clone().to_string();
        let new_password = new_password.clone();
        let admin_status = *admin_status.clone();
        let add_user_request = state.add_settings_user_reqeust.clone();
        let error_message_create = error_message.clone();
        let dispatch_wasm = dispatch.clone();
        Callback::from(move |e: MouseEvent| {
            let dispatch = dispatch_wasm.clone();
            let new_username = new_username.clone();
            let new_password = new_password.clone();
            let fullname = fullname.clone();
            let admin_status = admin_status.clone();
            let hash_pw = String::new();
            let salt = String::new();
            let email = email.clone();
            let page_state = page_state.clone();
            let error_message_clone = error_message_create.clone();
            e.prevent_default();
            page_state.set(PageState::Hidden);
            // Hash the password and generate a salt
            match validate_user_input(&new_username, &new_password, &email) {
                Ok(_) => {
                    match encode_password(&new_password) {
                        Ok((hash_pw, salt)) => {
                                        // Set the state
                            dispatch.reduce_mut(move |state| {
                                state.edit_settings_user_reqeust = Some(EditSettingsUserRequest {
                                    fullname,
                                    new_username,
                                    email,
                                    hash_pw,
                                    salt,
                                    admin_status
                                });
                            });

                            let add_user_request = add_user_request.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match call_add_user("http://localhost:8040".to_string(), &add_user_request).await {
                                    Ok(success) => {
                                        if success {
                                            console::log_1(&"User added successfully".into());
                                            page_state.set(PageState::Hidden);
                                        } else {
                                            console::log_1(&"Error adding user".into());
                                            page_state.set(PageState::Hidden);
                                            dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error adding user")));

                                        }
                                    }
                                    Err(e) => {
                                        console::log_1(&format!("Error: {}", e).into());
                                        page_state.set(PageState::Hidden);
                                        dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error adding user: {:?}", e)));
                                    }
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
                                <label for="password" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Password"}</label>
                                <input oninput={on_password_change.clone()} type="password" id="password" name="password" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="fullname" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Full Name"}</label>
                                <input oninput={on_fullname_change} type="text" id="fullname" name="fullname" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="email" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Email"}</label>
                                <input oninput={on_email_change} type="email" id="email" name="email" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
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
                <p class="text-lg font-bold mb-4">{"User Management:"}</p>
                <p class="text-md mb-4">{"You can manage users here. Click a user in the table to manage settings for that existing user or click 'Create New' to add a new user. Note that the guest user will always show regardless of whether it's enabled or not. View the Guest Settings Area to properly manage that."}</p>
                <button onclick={on_create_new_user} class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
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
                    <tbody onclick={on_edit_user}>
                        { for users.borrow().iter().map(|user| html! {
                            <tr class="table-row border-b cursor-pointer">
                                <td class="px-6 py-4">{ user.UserID }</td>
                                <td class="px-6 py-4">{ &user.Fullname }</td>
                                <td class="px-6 py-4">{ &user.Email }</td>
                                <td class="px-6 py-4">{ &user.Username }</td>
                                <td class="px-6 py-4">{ if user.IsAdmin == 1 { "Yes" } else { "No" } }</td>
                            </tr>
                        })}
                    </tbody>
                </table>
            </div>
        </>
    }
}