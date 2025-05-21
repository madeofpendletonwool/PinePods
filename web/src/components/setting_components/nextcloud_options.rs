use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::pod_req::connect_to_episode_websocket;
use crate::requests::setting_reqs::{
    call_add_gpodder_server, call_add_nextcloud_server, call_check_nextcloud_server,
    call_create_gpodder_device, call_force_full_sync, call_get_default_gpodder_device,
    call_get_gpodder_api_status, call_get_gpodder_devices, call_get_nextcloud_server,
    call_remove_podcast_sync, call_set_default_gpodder_device, call_sync_with_gpodder,
    call_test_gpodder_connection, call_toggle_gpodder_api, call_verify_gpodder_auth,
    initiate_nextcloud_login, CreateDeviceRequest, GpodderAuthRequest, GpodderCheckRequest,
    GpodderDevice, NextcloudAuthRequest,
};
use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use web_sys::HtmlSelectElement;
use yew::prelude::*;
use yewdux::use_store;

#[function_component(GpodderAdvancedOptions)]
pub fn gpodder_advanced_options() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    // State for the devices list
    let devices = use_state(|| Vec::<GpodderDevice>::new());

    // State for new device form
    let new_device_name = use_state(|| String::new());
    let new_device_type = use_state(|| "desktop".to_string());
    let new_device_caption = use_state(|| String::new());

    // Loading states
    let is_loading_devices = use_state(|| false);
    let is_creating_device = use_state(|| false);
    let is_syncing = use_state(|| false);
    let is_pushing = use_state(|| false);
    let is_setting_default = use_state(|| false);

    // Selected device for operations
    let selected_device_id = use_state(|| None::<i32>);

    // Add a state to store selected device info for operations
    let selected_device_info = use_state(|| None::<(i32, String, bool)>); // (id, name, is_remote)

    // Add state for default device
    let default_device = use_state(|| None::<GpodderDevice>);
    let is_loading_default_device = use_state(|| false);

    // Load default device on component mount
    {
        let default_device = default_device.clone();
        let is_loading_default_device = is_loading_default_device.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let dispatch = dispatch.clone();

        use_effect_with((), move |_| {
            if let (Some(server_name), Some(api_key)) = (server_name, api_key.clone()) {
                is_loading_default_device.set(true);

                spawn_local(async move {
                    match call_get_default_gpodder_device(&server_name, &api_key.unwrap()).await {
                        Ok(device) => {
                            default_device.set(Some(device));
                        }
                        Err(e) => {
                            // It's okay if no default device is set
                            if e.to_string().contains("404") {
                                default_device.set(None);
                            } else {
                                let error_msg =
                                    format!("Failed to load default GPodder device: {}", e);
                                dispatch.reduce_mut(|state| {
                                    state.error_message = Some(error_msg);
                                });
                            }
                        }
                    }

                    is_loading_default_device.set(false);
                });
            }

            || ()
        });
    }

    // Load devices on component mount
    {
        let devices = devices.clone();
        let is_loading_devices = is_loading_devices.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let dispatch = dispatch.clone();
        let selected_device_id = selected_device_id.clone();
        let selected_device_info = selected_device_info.clone();
        let default_device = default_device.clone();
        let default_device_clone = default_device.clone();

        use_effect_with((), move |_| {
            if let (Some(server_name), Some(api_key), Some(user_id)) =
                (server_name, api_key.clone(), user_id)
            {
                is_loading_devices.set(true);

                spawn_local(async move {
                    match call_get_gpodder_devices(&server_name, &api_key.unwrap(), user_id).await {
                        Ok(fetched_devices) => {
                            // Try to find the default device in the list
                            if let Some(default) = fetched_devices
                                .iter()
                                .find(|d| d.is_default.unwrap_or(false))
                            {
                                // If a default device exists, select it
                                selected_device_id.set(Some(default.id));
                                selected_device_info.set(Some((
                                    default.id,
                                    default.name.clone(),
                                    default.is_remote.unwrap_or(false),
                                )));

                                // Update the default_device state
                                default_device_clone.set(Some(default.clone()));
                            }
                            // Otherwise, if devices exist, select the first one by default
                            else if !fetched_devices.is_empty() {
                                let first_device = &fetched_devices[0];
                                selected_device_id.set(Some(first_device.id));
                                // Also store the device name and remote status
                                selected_device_info.set(Some((
                                    first_device.id,
                                    first_device.name.clone(),
                                    first_device.is_remote.unwrap_or(false),
                                )));
                            }
                            devices.set(fetched_devices);
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to load GPodder devices: {}", e);
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(error_msg);
                            });
                        }
                    }

                    is_loading_devices.set(false);
                });
            }

            || ()
        });
    }

    // Handler for device name input change
    let on_device_name_change = {
        let new_device_name = new_device_name.clone();

        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                new_device_name.set(input.value());
            }
        })
    };

    // Handler for device type selection change
    let on_device_type_change = {
        let new_device_type = new_device_type.clone();

        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<HtmlInputElement>() {
                new_device_type.set(select.value());
            }
        })
    };

    // Handler for device caption input change
    let on_device_caption_change = {
        let new_device_caption = new_device_caption.clone();

        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                new_device_caption.set(input.value());
            }
        })
    };

    // Updated device selection handler
    let on_device_select_change = {
        let selected_device_id = selected_device_id.clone();
        let selected_device_info = selected_device_info.clone();
        let devices = devices.clone();

        Callback::from(move |e: Event| {
            // Cast to proper HtmlSelectElement
            if let Some(select) = e.target_dyn_into::<HtmlSelectElement>() {
                let value = select.value();

                if value.is_empty() {
                    selected_device_id.set(None);
                    selected_device_info.set(None);
                } else {
                    // Parse the device ID
                    if let Ok(id) = value.parse::<i32>() {
                        // Find the selected device in the devices list
                        if let Some(device) = devices.iter().find(|d| d.id == id) {
                            web_sys::console::log_1(
                                &format!(
                                    "Selected device: {} (ID: {}, remote: {:?})",
                                    device.name, id, device.is_remote
                                )
                                .into(),
                            );

                            // Update both state variables
                            selected_device_id.set(Some(id));
                            selected_device_info.set(Some((
                                id,
                                device.name.clone(),
                                device.is_remote.unwrap_or(false),
                            )));
                        } else {
                            web_sys::console::log_1(
                                &format!("Could not find device with ID: {}", id).into(),
                            );
                        }
                    } else {
                        web_sys::console::log_1(
                            &format!("Failed to parse device ID: {}", value).into(),
                        );
                    }
                }
            }
        })
    };

    // Handler for setting default device
    let on_set_default_device = {
        let selected_device_info = selected_device_info.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let dispatch = dispatch.clone();
        let is_setting_default = is_setting_default.clone();
        let default_device = default_device.clone();
        let devices = devices.clone();

        Callback::from(move |_| {
            if let (Some(server_name), Some(api_key), Some((device_id, device_name, is_remote))) = (
                server_name.clone(),
                api_key.clone(),
                selected_device_info.as_ref().cloned(),
            ) {
                let is_set_default = is_setting_default.clone();
                is_set_default.set(true);
                let devices_clone = devices.clone();
                let default_device_clone = default_device.clone();
                let dispatch_clone = dispatch.clone();

                web_sys::console::log_1(
                    &format!(
                        "Setting device {} ({}) as default, is_remote: {}",
                        device_id, device_name, is_remote
                    )
                    .into(),
                );

                spawn_local(async move {
                    // Pass device name and is_remote status for remote devices (negative IDs)
                    match call_set_default_gpodder_device(
                        &server_name,
                        &api_key.unwrap(),
                        device_id,
                        Some(device_name.clone()),
                        is_remote,
                    )
                    .await
                    {
                        Ok(_response) => {
                            web_sys::console::log_1(
                                &format!("Successfully set device as default").into(),
                            );
                            // Find the device in our list and set it as default
                            if let Some(device) = devices_clone.iter().find(|d| d.id == device_id) {
                                let mut device_clone = device.clone();
                                device_clone.is_default = Some(true);
                                default_device_clone.set(Some(device_clone));

                                dispatch_clone.reduce_mut(|state| {
                                    state.info_message =
                                        Some("Default GPodder device set successfully".to_string());
                                });
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error setting default device: {}", e).into(),
                            );
                            let error_msg = format!("Failed to set default device: {}", e);
                            dispatch_clone.reduce_mut(|state| {
                                state.error_message = Some(error_msg);
                            });
                        }
                    }
                    is_set_default.set(false);
                });
            }
        })
    };

    // Handler for creating a new device
    let on_create_device = {
        let new_device_name = new_device_name.clone();
        let new_device_type = new_device_type.clone();
        let new_device_caption = new_device_caption.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let devices = devices.clone();
        let is_creating_device = is_creating_device.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |_| {
            let device_name = (*new_device_name).clone();
            let device_type = (*new_device_type).clone();
            let device_caption = (*new_device_caption).clone();
            let create_device = is_creating_device.clone();

            if device_name.is_empty() {
                dispatch.reduce_mut(|state| {
                    state.error_message = Some("Device name cannot be empty".to_string());
                });
                return;
            }

            if let (Some(server_name), Some(api_key), Some(user_id)) =
                (server_name.clone(), api_key.clone(), user_id)
            {
                is_creating_device.set(true);

                let request = CreateDeviceRequest {
                    user_id,
                    device_name,
                    device_type,
                    device_caption: if device_caption.is_empty() {
                        None
                    } else {
                        Some(device_caption)
                    },
                };

                let devices_clone = devices.clone();
                let new_device_name_clone = new_device_name.clone();
                let new_device_caption_clone = new_device_caption.clone();
                let dispatch_clone = dispatch.clone();

                spawn_local(async move {
                    match call_create_gpodder_device(&server_name, &api_key.unwrap(), request).await
                    {
                        Ok(new_device) => {
                            // Add the new device to the list
                            let mut updated_devices = (*devices_clone).clone();
                            updated_devices.push(new_device);
                            devices_clone.set(updated_devices);

                            // Clear the form
                            new_device_name_clone.set(String::new());
                            new_device_caption_clone.set(String::new());

                            dispatch_clone.reduce_mut(|state| {
                                state.info_message =
                                    Some("Device created successfully".to_string());
                            });
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to create device: {}", e);
                            dispatch_clone.reduce_mut(|state| {
                                state.error_message = Some(error_msg);
                            });
                        }
                    }

                    create_device.set(false);
                });
            }
        })
    };

    // Updated handler for syncing with GPodder
    let on_sync_click = {
        let selected_device_info = selected_device_info.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let is_syncing = is_syncing.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |_| {
            let is_sync = is_syncing.clone();
            if let (Some(server_name), Some(api_key), Some(user_id)) =
                (server_name.clone(), api_key.clone(), user_id)
            {
                // Get device info from the selected_device_info state
                if let Some((device_id, device_name, is_remote)) =
                    selected_device_info.as_ref().cloned()
                {
                    // Log the device being used for sync
                    web_sys::console::log_1(
                        &format!(
                            "Syncing with device: {} (ID: {}, remote: {})",
                            device_name, device_id, is_remote
                        )
                        .into(),
                    );

                    is_sync.set(true);
                    let dispatch_clone = dispatch.clone();

                    spawn_local(async move {
                        match call_sync_with_gpodder(
                            &server_name,
                            &api_key.clone().unwrap(),
                            user_id,
                            Some(device_id),   // Pass the device ID
                            Some(device_name), // Pass the device name
                            is_remote,         // Pass the remote status
                        )
                        .await
                        {
                            Ok(response) => {
                                if response.success {
                                    dispatch_clone.reduce_mut(|state| {
                                        state.info_message = Some(response.message);
                                        state.is_refreshing = Some(true);
                                    });

                                    // Optionally refresh podcasts via websocket
                                    if let Err(e) = connect_to_episode_websocket(
                                        &server_name,
                                        &user_id,
                                        &api_key.clone().unwrap(),
                                        true,
                                        dispatch_clone.clone(),
                                    )
                                    .await
                                    {
                                        web_sys::console::log_1(
                                            &format!("Failed to connect to WebSocket: {:?}", e)
                                                .into(),
                                        );
                                    }

                                    dispatch_clone.reduce_mut(|state| {
                                        state.is_refreshing = Some(false);
                                    });
                                } else {
                                    dispatch_clone.reduce_mut(|state| {
                                        state.error_message = Some(response.message);
                                    });
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to sync with GPodder: {}", e);
                                dispatch_clone.reduce_mut(|state| {
                                    state.error_message = Some(error_msg);
                                });
                            }
                        }

                        is_sync.set(false);
                    });
                } else {
                    // Display error if no device is selected
                    dispatch.reduce_mut(|state| {
                        state.error_message =
                            Some("No device selected for synchronization".to_string());
                    });
                }
            }
        })
    };

    // Updated handler for pushing all podcasts to GPodder
    let on_push_click = {
        let selected_device_info = selected_device_info.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let is_pushing = is_pushing.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |_| {
            let is_pushing = is_pushing.clone();
            if let (Some(server_name), Some(api_key), Some(user_id)) =
                (server_name.clone(), api_key.clone(), user_id)
            {
                // Get device info from the selected_device_info state
                if let Some((device_id, device_name, is_remote)) =
                    selected_device_info.as_ref().cloned()
                {
                    // Log the device being used for push
                    web_sys::console::log_1(
                        &format!(
                            "Pushing to device: {} (ID: {}, remote: {})",
                            device_name, device_id, is_remote
                        )
                        .into(),
                    );

                    is_pushing.set(true);
                    let dispatch_clone = dispatch.clone();

                    spawn_local(async move {
                        match call_force_full_sync(
                            &server_name,
                            &api_key.unwrap(),
                            user_id,
                            Some(device_id),
                            Some(device_name),
                            is_remote,
                        )
                        .await
                        {
                            Ok(response) => {
                                if response.success {
                                    dispatch_clone.reduce_mut(|state| {
                                        state.info_message = Some(response.message);
                                    });
                                } else {
                                    dispatch_clone.reduce_mut(|state| {
                                        state.error_message = Some(response.message);
                                    });
                                }
                            }
                            Err(e) => {
                                let error_msg =
                                    format!("Failed to push podcasts to GPodder: {}", e);
                                dispatch_clone.reduce_mut(|state| {
                                    state.error_message = Some(error_msg);
                                });
                            }
                        }
                        is_pushing.set(false);
                    });
                } else {
                    // Display error if no device is selected
                    dispatch.reduce_mut(|state| {
                        state.error_message =
                            Some("No device selected for pushing podcasts".to_string());
                    });
                }
            }
        })
    };

    // Determine if the currently selected device is the default
    let is_selected_device_default = {
        if let (Some(device_id), Some(default)) = (*selected_device_id, default_device.as_ref()) {
            device_id == default.id
        } else {
            false
        }
    };

    // Render the component
    html! {
        <div class="p-4">
            <h2 class="item_container-text text-lg font-bold mb-4">{"GPodder Advanced Settings"}</h2>

            <div class="mb-6">
                <p class="item_container-text text-md mb-4">
                    {"GPodder synchronization allows you to manage your podcast subscriptions across multiple devices. Here you can manage devices registered with your GPodder account and control synchronization."}
                </p>
            </div>

            // Devices section
            <div class="mb-8 p-4 border rounded-lg">
                <h3 class="item_container-text text-md font-bold mb-4">{"Your GPodder Devices"}</h3>

                {
                    if *is_loading_devices {
                        html! { <div class="flex items-center mb-4"><i class="ph ph-spinner animate-spin mr-2"></i><span>{"Loading devices..."}</span></div> }
                    } else if devices.is_empty() {
                        html! { <p class="text-md mb-4">{"No devices found. Create your first device below."}</p> }
                    } else {
                        html! {
                            <>
                                <div class="mb-4">
                                    <label for="device-select" class="block text-sm font-medium mb-2">{"Select device for operations:"}</label>
                                    <select
                                        id="device-select"
                                        class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
                                        onchange={on_device_select_change}
                                    >
                                        <option value="">{"-- Select a device --"}</option>
                                        {
                                            devices.iter().map(|device| {
                                                let caption = match &device.caption {
                                                    Some(c) => format!(" ({})", c),
                                                    None => String::new()
                                                };
                                                let selected = match *selected_device_id {
                                                    Some(id) if id == device.id => true,
                                                    _ => false
                                                };
                                                let is_default = if let Some(default) = default_device.as_ref() {
                                                    default.id == device.id
                                                } else {
                                                    false
                                                };
                                                let device_label = if is_default {
                                                    format!("{}{} - {} [DEFAULT]", device.name, caption, device.r#type)
                                                } else {
                                                    format!("{}{} - {}", device.name, caption, device.r#type)
                                                };

                                                html! {
                                                    <option value={device.id.to_string()} selected={selected}>
                                                        {device_label}
                                                    </option>
                                                }
                                            }).collect::<Html>()
                                        }
                                    </select>
                                </div>

                                <div class="flex flex-wrap gap-3 mb-4">
                                    <button
                                        onclick={on_sync_click}
                                        disabled={*is_syncing || selected_device_id.is_none()}
                                        class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                                    >
                                        {
                                            if *is_syncing {
                                                html! { <span class="flex items-center"><i class="ph ph-spinner animate-spin mr-2"></i>{"Syncing..."}</span> }
                                            } else {
                                                html! { <span class="flex items-center"><i class="ph ph-arrow-down-from-line mr-2"></i>{"Sync from GPodder"}</span> }
                                            }
                                        }
                                    </button>

                                    <button
                                        onclick={on_push_click}
                                        disabled={*is_pushing || selected_device_id.is_none()}
                                        class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                                    >
                                        {
                                            if *is_pushing {
                                                html! { <span class="flex items-center"><i class="ph ph-spinner animate-spin mr-2"></i>{"Pushing..."}</span> }
                                            } else {
                                                html! { <span class="flex items-center"><i class="ph ph-arrow-up-from-line mr-2"></i>{"Push to GPodder"}</span> }
                                            }
                                        }
                                    </button>

                                    <button
                                        onclick={on_set_default_device}
                                        disabled={*is_setting_default || selected_device_id.is_none() || is_selected_device_default}
                                        class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                                    >
                                        {
                                            if *is_setting_default {
                                                html! { <span class="flex items-center"><i class="ph ph-spinner animate-spin mr-2"></i>{"Setting default..."}</span> }
                                            } else if is_selected_device_default {
                                                html! { <span class="flex items-center"><i class="ph ph-check-circle mr-2"></i>{"Current default"}</span> }
                                            } else {
                                                html! { <span class="flex items-center"><i class="ph ph-star mr-2"></i>{"Set as default"}</span> }
                                            }
                                        }
                                    </button>
                                </div>

                                <div class="mb-4">
                                    {
                                        if let Some(default_dev) = default_device.as_ref() {
                                            html! {
                                                <div class="flex items-center py-2 px-4 bg-gray-100 dark:bg-gray-700 rounded-lg">
                                                    <i class="ph ph-info text-blue-500 mr-2"></i>
                                                    <span>
                                                        {format!("Default device: {}", default_dev.name)}
                                                        {
                                                            if let Some(caption) = &default_dev.caption {
                                                                format!(" ({})", caption)
                                                            } else {
                                                                String::new()
                                                            }
                                                        }
                                                    </span>
                                                </div>
                                            }
                                        } else if *is_loading_default_device {
                                            html! {
                                                <div class="flex items-center">
                                                    <i class="ph ph-spinner animate-spin mr-2"></i>
                                                    <span>{"Loading default device..."}</span>
                                                </div>
                                            }
                                        } else {
                                            html! {
                                                <div class="flex items-center py-2 px-4 bg-gray-100 dark:bg-gray-700 rounded-lg">
                                                    <i class="ph ph-info text-yellow-500 mr-2"></i>
                                                    <span>{"No default device set. Select a device and click 'Set as default'."}</span>
                                                </div>
                                            }
                                        }
                                    }
                                </div>

                                <table class="w-full text-sm text-left">
                                    <thead class="text-xs uppercase">
                                        <tr>
                                            <th class="py-3 px-6">{"Name"}</th>
                                            <th class="py-3 px-6">{"Type"}</th>
                                            <th class="py-3 px-6">{"Last Sync"}</th>
                                            <th class="py-3 px-6">{"Status"}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {
                                            devices.iter().map(|device| {
                                                let is_default = if let Some(default) = default_device.as_ref() {
                                                    default.id == device.id
                                                } else {
                                                    false
                                                };

                                                html! {
                                                    <tr class="border-b">
                                                        <td class="py-4 px-6">
                                                            {&device.name}
                                                            {
                                                                if let Some(caption) = &device.caption {
                                                                    html! { <span class="text-gray-500 ml-2">{"("}{caption}{")"}</span> }
                                                                } else {
                                                                    html! {}
                                                                }
                                                            }
                                                        </td>
                                                        <td class="py-4 px-6">{&device.r#type}</td>
                                                        <td class="py-4 px-6">
                                                            {
                                                                if let Some(last_sync) = &device.last_sync {
                                                                    html! { {last_sync} }
                                                                } else {
                                                                    html! { {"Never"} }
                                                                }
                                                            }
                                                        </td>
                                                        <td class="py-4 px-6">
                                                            {
                                                                if is_default {
                                                                    html! {
                                                                        <span class="flex items-center text-green-500">
                                                                            <i class="ph ph-star-fill mr-1"></i>
                                                                            {"Default"}
                                                                        </span>
                                                                    }
                                                                } else {
                                                                    html! { {"—"} }
                                                                }
                                                            }
                                                        </td>
                                                    </tr>
                                                }
                                            }).collect::<Html>()
                                        }
                                    </tbody>
                                </table>
                            </>
                        }
                    }
                }
            </div>

            // Create device section
            <div class="p-4 border rounded-lg">
                <h3 class="item_container-text text-md font-bold mb-4">{"Add New Device"}</h3>

                <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
                    <div>
                        <label for="device-name" class="block text-sm font-medium mb-2">{"Device Name (required)"}</label>
                        <input
                            type="text"
                            id="device-name"
                            class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
                            placeholder="my-phone"
                            value={(*new_device_name).clone()}
                            oninput={on_device_name_change}
                        />
                    </div>

                    <div>
                        <label for="device-type" class="block text-sm font-medium mb-2">{"Device Type"}</label>
                        <select
                            id="device-type"
                            class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
                            onchange={on_device_type_change}
                        >
                            <option value="desktop" selected={*new_device_type == "desktop"}>{"Desktop"}</option>
                            <option value="laptop" selected={*new_device_type == "laptop"}>{"Laptop"}</option>
                            <option value="mobile" selected={*new_device_type == "mobile"}>{"Mobile"}</option>
                            <option value="server" selected={*new_device_type == "server"}>{"Server"}</option>
                            <option value="other" selected={*new_device_type == "other"}>{"Other"}</option>
                        </select>
                    </div>

                    <div>
                        <label for="device-caption" class="block text-sm font-medium mb-2">{"Caption (optional)"}</label>
                        <input
                            type="text"
                            id="device-caption"
                            class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
                            placeholder="My Android Phone"
                            value={(*new_device_caption).clone()}
                            oninput={on_device_caption_change}
                        />
                    </div>
                </div>

                <div class="mt-4">
                    <button
                        onclick={on_create_device}
                        disabled={*is_creating_device || (*new_device_name).is_empty()}
                        class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                    >
                        {
                            if *is_creating_device {
                                html! { <span class="flex items-center"><i class="ph ph-spinner animate-spin mr-2"></i>{"Creating..."}</span> }
                            } else {
                                html! { <span class="flex items-center"><i class="ph ph-plus mr-2"></i>{"Add Device"}</span> }
                            }
                        }
                    </button>
                </div>
            </div>

            // Add help section about default devices
            <div class="mt-6 p-4 border rounded-lg bg-gray-50 dark:bg-gray-800">
                <h3 class="item_container-text text-md font-bold mb-2">{"About Default Devices"}</h3>
                <p class="text-sm mb-2">
                    {"Setting a default device simplifies GPodder synchronization by automatically selecting it for sync operations. This is especially useful if you primarily sync with one device."}
                </p>
                <p class="text-sm mb-2">
                    {"When a default device is set:"}
                </p>
                <ul class="list-disc pl-5 mb-2 text-sm">
                    <li>{"It will be pre-selected in the device dropdown"}</li>
                    <li>{"Background sync operations will use this device"}</li>
                    <li>{"You can still manually select other devices when needed"}</li>
                </ul>
                <p class="text-sm italic">
                    {"Note: You can change your default device at any time by selecting a different device and clicking 'Set as default'."}
                </p>
            </div>
        </div>
    }
}

// Assume this struct is for handling the response of the initial login request
#[derive(Serialize, Deserialize)]
pub struct NextcloudLoginResponse {
    pub poll: Poll,
    pub login: String,
}

#[derive(Serialize, Deserialize)]
pub struct Poll {
    pub token: String,
    pub endpoint: String,
}

async fn open_nextcloud_login(url: &str) -> Result<(), JsValue> {
    let window = web_sys::window().expect("no global `window` exists");
    window.open_with_url_and_target(url, "_blank")?;
    Ok(())
}

#[function_component(SyncOptions)]
pub fn sync_options() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let server_url = use_state(|| String::new());
    let server_user = use_state(|| String::new());
    let server_pass = use_state(|| String::new());
    let auth_status = use_state(|| String::new());
    let nextcloud_url = use_state(|| String::new()); // State to hold the Nextcloud server URL
    let _error_message = state.error_message.clone();
    let _info_message = state.info_message.clone();
    let is_internal_gpodder_enabled = use_state(|| false);
    let is_toggling_gpodder = use_state(|| false);

    // State to track if sync is configured
    let is_sync_configured = use_state(|| false);

    // State to track the sync type
    let sync_type = use_state(|| "None".to_string());

    // State to determine if we should show advanced options
    let show_advanced_options = use_state(|| false);

    // Loading states
    let is_loading = use_state(|| false);
    let is_testing_connection = use_state(|| false);

    // Effect to get current gpodder API status
    {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let is_internal_gpodder_enabled = is_internal_gpodder_enabled.clone();
        let dispatch = dispatch.clone();
        let sync_type = sync_type.clone();

        use_effect_with(&(), move |_| {
            if let (Some(server_name), Some(api_key)) = (server_name.clone(), api_key.clone()) {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_get_gpodder_api_status(&server_name, &api_key.unwrap()).await {
                        Ok(status) => {
                            is_internal_gpodder_enabled.set(status.gpodder_enabled);
                            // Set the sync type from the API response
                            sync_type.set(status.sync_type);
                        }
                        Err(e) => {
                            let error_msg = format!("Error fetching gpodder API status: {}", e);
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(error_msg);
                            });
                        }
                    }
                });
            }
            || ()
        });
    }

    // Handler for server URL input change
    let on_server_url_change = {
        let server_url = server_url.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                server_url.set(input.value());
            }
        })
    };

    let on_username_change = {
        let server_user = server_user.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                server_user.set(input.value());
            }
        })
    };

    let on_password_change = {
        let server_pass = server_pass.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                server_pass.set(input.value());
            }
        })
    };

    // Effect to get current sync status
    {
        let nextcloud_url = nextcloud_url.clone();
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        let is_sync_configured = is_sync_configured.clone();
        let sync_type = sync_type.clone();

        use_effect_with(&(), move |_| {
            let nextcloud_url = nextcloud_url.clone();
            let user_id = user_id.clone().unwrap_or_default();

            wasm_bindgen_futures::spawn_local(async move {
                match call_get_nextcloud_server(
                    &server_name.clone().unwrap(),
                    &api_key.clone().unwrap().unwrap(),
                    user_id,
                )
                .await
                {
                    Ok(server) => {
                        if !server.is_empty()
                            && server != "Not currently syncing with Nextcloud server"
                            && server != "Not currently syncing with any server"
                        {
                            nextcloud_url.set(server);
                            is_sync_configured.set(true);

                            // Get the sync type
                            let sync_type_clone = sync_type.clone();
                            if let (Some(_server_name), Some(_api_key)) =
                                (server_name.clone(), api_key.clone())
                            {
                                wasm_bindgen_futures::spawn_local(async move {
                                    // This would be a new API call to get the sync type
                                    // For now, we'll assume we know it's either "gpodder" or "nextcloud"
                                    // This would be replaced with the actual API call

                                    // Placeholder logic - replace with actual API call
                                    if nextcloud_url.contains("nextcloud") {
                                        sync_type_clone.set("nextcloud".to_string());
                                    } else {
                                        sync_type_clone.set("gpodder".to_string());
                                    }
                                });
                            }
                        } else {
                            nextcloud_url
                                .set(String::from("Not currently syncing with any server"));
                            is_sync_configured.set(false);
                            sync_type.set("None".to_string());
                        }
                    }
                    Err(_) => {
                        nextcloud_url.set(String::from("Not currently syncing with any server"));
                        is_sync_configured.set(false);
                        sync_type.set("None".to_string());
                    }
                }
            });

            || () // Return empty cleanup function
        });
    }

    // Update the on_toggle_internal_gpodder callback to properly handle the response:
    let on_toggle_internal_gpodder = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let is_internal_gpodder_enabled = is_internal_gpodder_enabled.clone();
        let is_toggling_gpodder = is_toggling_gpodder.clone();
        let dispatch = dispatch.clone();
        let nextcloud_url = nextcloud_url.clone();
        let sync_type = sync_type.clone();
        let is_sync_configured = is_sync_configured.clone();

        Callback::from(move |_| {
            let is_internal_gpodder_enabled = is_internal_gpodder_enabled.clone();
            let is_toggling_gpodder = is_toggling_gpodder.clone();
            let dispatch = dispatch.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let new_state = !(*is_internal_gpodder_enabled);
            let next_url = nextcloud_url.clone();
            let call_sync_type = sync_type.clone();
            let sync_config = is_sync_configured.clone();

            is_toggling_gpodder.set(true);

            wasm_bindgen_futures::spawn_local(async move {
                match call_toggle_gpodder_api(
                    &server_name.unwrap(),
                    &api_key.unwrap().unwrap(),
                    new_state,
                )
                .await
                {
                    Ok(status) => {
                        // Update local state based on response
                        is_internal_gpodder_enabled.set(status.gpodder_enabled);

                        // Force UI refresh by setting sync status
                        if status.gpodder_enabled {
                            next_url.set(String::from("Internal gpodder API enabled"));
                            call_sync_type.set("gpodder".to_string());
                            sync_config.set(true);
                        } else {
                            next_url.set(String::from("Not currently syncing with any server"));
                            call_sync_type.set("None".to_string());
                            sync_config.set(false);
                        }

                        let message = if status.gpodder_enabled {
                            "Internal gpodder API enabled"
                        } else {
                            "Internal gpodder API disabled"
                        };

                        dispatch.reduce_mut(|state| {
                            state.info_message = Some(message.to_string());
                        });
                    }
                    Err(e) => {
                        let error_msg = format!("Error toggling gpodder API: {}", e);
                        dispatch.reduce_mut(|state| {
                            state.error_message = Some(error_msg);
                        });
                    }
                }
                is_toggling_gpodder.set(false);
            });
        })
    };

    // Handler for toggling advanced options
    let on_toggle_advanced = {
        let show_advanced_options = show_advanced_options.clone();

        Callback::from(move |_| {
            show_advanced_options.set(!*show_advanced_options);
        })
    };

    // Handler for initiating authentication
    let on_authenticate_click = {
        let dispatch = dispatch.clone();
        let server_url = server_url.clone();
        let server_url_initiate = server_url.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let auth_status = auth_status.clone();

        Callback::from(move |_| {
            let dispatch = dispatch.clone();
            let auth_status = auth_status.clone();
            let server = (*server_url_initiate).clone().trim().to_string();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let dispatch_clone = dispatch.clone();

            if !server.trim().is_empty() {
                wasm_bindgen_futures::spawn_local(async move {
                    match initiate_nextcloud_login(
                        &server,
                        &server_name.clone().unwrap(),
                        &api_key.clone().unwrap().unwrap(),
                        user_id.clone().unwrap(),
                    )
                    .await
                    {
                        Ok(login_data) => {
                            match open_nextcloud_login(&login_data.login).await {
                                Ok(_) => println!("Opened login URL in new tab"),
                                Err(e) => println!("Failed to open login URL in new tab: {:?}", e),
                            }
                            // Use login_data.poll_endpoint and login_data.token for the next steps
                            let auth_request = NextcloudAuthRequest {
                                user_id: user_id.clone().unwrap(),
                                token: login_data.poll.token,
                                poll_endpoint: login_data.poll.endpoint,
                                nextcloud_url: server.clone(),
                            };
                            match call_add_nextcloud_server(
                                &server_name.clone().unwrap(),
                                &api_key.clone().unwrap().unwrap(),
                                auth_request,
                            )
                            .await
                            {
                                Ok(_) => {
                                    log::info!("pinepods server now polling nextcloud");
                                    // Start polling the check_gpodder_settings endpoint
                                    loop {
                                        match call_check_nextcloud_server(
                                            &server_name.clone().unwrap(),
                                            &api_key.clone().unwrap().unwrap(),
                                            user_id.clone().unwrap(),
                                        )
                                        .await
                                        {
                                            Ok(response) => {
                                                if response.data {
                                                    log::info!("gPodder settings have been set up");
                                                    dispatch.reduce_mut(|state| state.info_message = Option::from("Nextcloud server has been authenticated successfully".to_string()));

                                                    // Set `is_refreshing` to true and start the WebSocket refresh
                                                    let server_name_call = server_name.clone();
                                                    let user_id_call = user_id.clone();
                                                    let api_key_call = api_key.clone();
                                                    dispatch_clone.reduce_mut(|state| {
                                                        state.is_refreshing = Some(true);
                                                        state.clone() // Return the modified state
                                                    });

                                                    spawn_local(async move {
                                                        if let Err(e) =
                                                            connect_to_episode_websocket(
                                                                &server_name_call.unwrap(),
                                                                &user_id_call.unwrap(),
                                                                &api_key_call.unwrap().unwrap(),
                                                                true,
                                                                dispatch_clone.clone(),
                                                            )
                                                            .await
                                                        {
                                                            web_sys::console::log_1(
                                                                &format!("Failed to connect to WebSocket: {:?}", e).into(),
                                                            );
                                                        } else {
                                                            web_sys::console::log_1(
                                                                &"WebSocket connection established and refresh initiated.".into(),
                                                            );
                                                        }

                                                        // Stop the loading animation after the WebSocket operation is complete
                                                        dispatch_clone.reduce_mut(|state| {
                                                            state.is_refreshing = Some(false);
                                                            state.clone() // Return the modified state
                                                        });
                                                    });

                                                    break;
                                                } else {
                                                    log::info!("gPodder settings are not yet set up, continuing to poll...");
                                                }
                                            }
                                            Err(e) => log::error!(
                                                "Error calling check_gpodder_settings: {:?}",
                                                e
                                            ),
                                        }

                                        // Wait for a short period before polling again
                                        let delay = std::time::Duration::from_secs(5);
                                        async_std::task::sleep(delay).await;
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error calling add_nextcloud_server: {:?}", e);
                                    let formatted_error = format_error_message(&e.to_string());
                                    dispatch.reduce_mut(|state| {
                                        state.error_message = Option::from(
                                            format!(
                                                "Error calling add_nextcloud_server: {}",
                                                formatted_error
                                            )
                                            .to_string(),
                                        )
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(&JsValue::from_str(&format!(
                                "Failed to initiate Nextcloud login: {:?}",
                                e
                            )));
                            dispatch.reduce_mut(|state| state.error_message = Option::from("Failed to initiate Nextcloud login. Please check the server URL.".to_string()));
                            auth_status.set(
                                "Failed to initiate Nextcloud login. Please check the server URL."
                                    .to_string(),
                            );
                        }
                    }
                });
            } else {
                auth_status.set("Please enter a Nextcloud server URL.".to_string());
                dispatch.reduce_mut(|state| {
                    state.error_message =
                        Option::from("Please enter a Nextcloud Server URL".to_string())
                });
            }
        })
    };

    let on_remove_sync_click = {
        let dispatch = dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let nextcloud_url = nextcloud_url.clone();
        let is_sync_configured = is_sync_configured.clone();
        let is_loading = is_loading.clone();
        let sync_type = sync_type.clone();

        Callback::from(move |_| {
            let dispatch = dispatch.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let nextcloud_url = nextcloud_url.clone();
            let is_sync_configured = is_sync_configured.clone();
            let is_loading = is_loading.clone();
            let sync_type = sync_type.clone();

            is_loading.set(true);

            wasm_bindgen_futures::spawn_local(async move {
                match call_remove_podcast_sync(
                    &server_name.clone().unwrap(),
                    &api_key.clone().unwrap().unwrap(),
                    user_id.clone().unwrap(),
                )
                .await
                {
                    Ok(success) => {
                        if success {
                            nextcloud_url
                                .set(String::from("Not currently syncing with any server"));
                            is_sync_configured.set(false);
                            sync_type.set("None".to_string());
                            dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Podcast sync settings removed successfully".to_string());
                            });
                        } else {
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some("Failed to remove sync settings".to_string());
                            });
                        }
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Error removing sync settings: {}", formatted_error));
                        });
                    }
                }
                is_loading.set(false);
            });
        })
    };

    // Handler for testing GPodder connection
    let on_test_connection = {
        let server_url = server_url.clone();
        let server_user = server_user.clone();
        let server_pass = server_pass.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let dispatch = dispatch.clone();
        let test_connect = is_testing_connection.clone();

        Callback::from(move |_| {
            let server_url = (*server_url).clone();
            let server_user = (*server_user).clone();
            let server_pass = (*server_pass).clone();
            let testing_connection = test_connect.clone();

            if server_url.is_empty() || server_user.is_empty() || server_pass.is_empty() {
                dispatch.reduce_mut(|state| {
                    state.error_message =
                        Some("Please enter server URL, username, and password".to_string());
                });
                return;
            }

            testing_connection.set(true);

            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let dispatch = dispatch.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match call_test_gpodder_connection(
                    &server_name.unwrap(),
                    &api_key.unwrap().unwrap(),
                    user_id.unwrap(),
                    &server_url,
                    &server_user,
                    &server_pass,
                )
                .await
                {
                    Ok(response) => {
                        if response.success {
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some(response.message);
                            });
                        } else {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(response.message);
                            });
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to test GPodder connection: {}", e);
                        dispatch.reduce_mut(|state| {
                            state.error_message = Some(error_msg);
                        });
                    }
                }

                testing_connection.set(false);
            });
        })
    };

    // Handler for initiating authentication to a gpodder server
    let on_authenticate_server_click = {
        let server_url = server_url.clone();
        let server_user = server_user.clone();
        let server_pass = server_pass.clone();
        let server_url_initiate = server_url.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let auth_status = auth_status.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |_| {
            let auth_status = auth_status.clone();
            let server = (*server_url_initiate).clone().trim().to_string();
            let server_user = server_user.clone();
            let server_pass = server_pass.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let dispatch_clone = dispatch.clone();
            let server_user_check_deref = (*server_user).clone();
            let server_user_deref = (*server_user).clone();
            let server_pass_check_deref = (*server_pass).clone();
            let server_pass_deref = (*server_pass).clone();

            if !server.trim().is_empty() {
                wasm_bindgen_futures::spawn_local(async move {
                    let auth_request = GpodderAuthRequest {
                        user_id: user_id.clone().unwrap(),
                        gpodder_url: server.clone(),
                        gpodder_username: server_user_deref,
                        gpodder_password: server_pass_deref,
                    };
                    let check_request = GpodderCheckRequest {
                        gpodder_url: server.clone(),
                        gpodder_username: server_user_check_deref,
                        gpodder_password: server_pass_check_deref,
                    };
                    match call_verify_gpodder_auth(&server_name.clone().unwrap(), check_request)
                        .await
                    {
                        Ok(auth_response) => {
                            if auth_response.status == "success" {
                                match call_add_gpodder_server(
                                    &server_name.clone().unwrap(),
                                    &api_key.clone().unwrap().unwrap(),
                                    auth_request,
                                )
                                .await
                                {
                                    Ok(_) => {
                                        log::info!(
                                            "Gpodder server now added and podcasts syncing!"
                                        );
                                        dispatch_clone.reduce_mut(|state| {
                                            state.info_message = Option::from(
                                                "Gpodder server now added and podcasts syncing!"
                                                    .to_string(),
                                            )
                                        });
                                        // Set `is_refreshing` to true and start the WebSocket refresh
                                        let server_name_call = server_name.clone();
                                        let user_id_call = user_id.clone();
                                        let api_key_call = api_key.clone();
                                        dispatch_clone.reduce_mut(|state| {
                                            state.is_refreshing = Some(true);
                                            state.clone() // Return the modified state
                                        });

                                        spawn_local(async move {
                                            if let Err(e) = connect_to_episode_websocket(
                                                &server_name_call.unwrap(),
                                                &user_id_call.unwrap(),
                                                &api_key_call.unwrap().unwrap(),
                                                true,
                                                dispatch_clone.clone(),
                                            )
                                            .await
                                            {
                                                web_sys::console::log_1(
                                                    &format!(
                                                        "Failed to connect to WebSocket: {:?}",
                                                        e
                                                    )
                                                    .into(),
                                                );
                                            } else {
                                                web_sys::console::log_1(
                                                    &"WebSocket connection established and refresh initiated.".into(),
                                                );
                                            }

                                            // Stop the loading animation after the WebSocket operation is complete
                                            dispatch_clone.reduce_mut(|state| {
                                                state.is_refreshing = Some(false);
                                                state.clone() // Return the modified state
                                            });
                                        });
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(&JsValue::from_str(&format!(
                                            "Failed to add Gpodder server: {:?}",
                                            e
                                        )));
                                        let formatted_error = format_error_message(&e.to_string());
                                        dispatch_clone.reduce_mut(|state| state.error_message = Option::from("Failed to add Gpodder server. Please check the server URL.".to_string()));
                                        auth_status.set(
                                            format!("Failed to add Gpodder server. Please check the server URL and credentials. {:?}", formatted_error)
                                                .to_string(),
                                        );
                                    }
                                }
                            } else {
                                web_sys::console::log_1(&JsValue::from_str(
                                    "Authentication failed.",
                                ));
                                dispatch_clone.reduce_mut(|state| {
                                    state.error_message = Option::from(
                                        "Authentication failed. Please check your credentials."
                                            .to_string(),
                                    )
                                });
                                auth_status.set(
                                    "Authentication failed. Please check your credentials."
                                        .to_string(),
                                );
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(&JsValue::from_str(&format!(
                                "Failed to verify Gpodder auth: {:?}",
                                e
                            )));
                            dispatch_clone.reduce_mut(|state| {
                                state.error_message = Option::from(
                                    "Failed to verify Gpodder auth. Please check the server URL."
                                        .to_string(),
                                )
                            });
                            auth_status.set(
                                "Failed to verify Gpodder auth. Please check the server URL."
                                    .to_string(),
                            );
                        }
                    }
                });
            } else {
                auth_status.set("Please enter a Gpodder server URL.".to_string());
                dispatch_clone.reduce_mut(|state| {
                    state.error_message =
                        Option::from("Please enter a Gpodder Server URL".to_string())
                });
            }
        })
    };

    let determine_sync_type = || {
        if *is_internal_gpodder_enabled && (*nextcloud_url) == "http://localhost:8042" {
            // Only consider it internal if BOTH gpodder_enabled is true AND the URL is localhost
            "internal_gpodder"
        } else if *sync_type == "nextcloud" {
            "nextcloud"
        } else if *is_sync_configured && *sync_type == "gpodder" {
            // External gpodder - when sync_type is gpodder but URL is not localhost
            "external_gpodder"
        } else {
            "none"
        }
    };

    // Determine if sync options should be hidden
    let should_hide_sync_options = *is_internal_gpodder_enabled || *sync_type == "nextcloud";

    html! {
        <div class="p-4">
            <p class="item_container-text text-lg font-bold mb-4">{"Podcast Sync Settings"}</p>
            <p class="item_container-text text-md mb-4">{"With this option you can authenticate with a Nextcloud or GPodder server to use as a podcast sync client. This works great with AntennaPod on Android so you can have the same exact feed there while on mobile. In addition, if you're already using AntennaPod with Nextcloud Podcast sync you can connect your existing sync feed to quickly import everything right into Pinepods! You'll only enter information for one of the below options. Nextcloud requires that you have the gpodder sync add-on in nextcloud and the gpodder option requires you to have an external gpodder podcast sync server that authenticates via user and pass."}</p>

            <div class="flex items-center mb-4">
                <p class="item_container-text text-md mr-4">{"Current Podcast Sync Server: "}
                    <span class="item_container-text font-bold">
                    {
                        if (*nextcloud_url) == "http://localhost:8042" {
                            "Internal Sync Server".to_string()
                        } else {
                            (*nextcloud_url).clone()
                        }
                    }
                    </span>
                </p>
                {
                    if *is_sync_configured {
                        html! {
                            <button
                                onclick={on_remove_sync_click}
                                disabled={*is_loading}
                                class="ml-4 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                            >
                            {
                                if *is_loading {
                                    html! { <span class="flex items-center"><i class="ph ph-spinner animate-spin mr-2"></i>{"Removing..."}</span> }
                                } else {
                                    html! { "Remove Sync" }
                                }
                            }
                            </button>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>

            <br/>

            // Internal Gpodder API Section
            {
                if !should_hide_sync_options {
                    html! {
                        <div class="mb-6 p-4 border rounded-lg">
                            <h3 class="item_container-text text-md font-bold mb-4">{"Internal Gpodder API"}</h3>
                            <p class="item_container-text text-sm mb-4">
                                {"Enable the internal gpodder API to synchronize podcasts between Pinepods and other gpodder-compatible clients. This will disable external sync options while enabled."}
                            </p>
                            <div class="flex items-center">
                                <label class="relative inline-flex items-center cursor-pointer">
                                    <input
                                        type="checkbox"
                                        class="sr-only peer"
                                        checked={*is_internal_gpodder_enabled}
                                        disabled={*is_toggling_gpodder}
                                        onclick={on_toggle_internal_gpodder}
                                    />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                    <span class="ml-3 text-sm font-medium text-gray-900 dark:text-gray-300">
                                        {
                                            if *is_toggling_gpodder {
                                                html! { <span class="flex items-center"><i class="ph ph-spinner animate-spin mr-2"></i>{"Processing..."}</span> }
                                            } else if *is_internal_gpodder_enabled {
                                                html! { "Enabled" }
                                            } else {
                                                html! { "Disabled" }
                                            }
                                        }
                                    </span>
                                </label>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Nextcloud Section - hide completely when internal API is enabled
            {
                if !should_hide_sync_options {
                    html! {
                        <div class="mb-6 p-4 border rounded-lg">
                            <h3 class="item_container-text text-md font-bold mb-4">{"Nextcloud Sync"}</h3>
                            <label for="server_url" class="item_container-text block mb-2 text-sm font-medium">{ "Nextcloud Server URL" }</label>
                            <div class="flex items-center">
                                <input
                                    type="text"
                                    id="nextcloud_url"
                                    oninput={on_server_url_change.clone()}
                                    class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
                                    placeholder="https://nextcloud.com"
                                />
                                <button
                                    onclick={on_authenticate_click}
                                    class="ml-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                                >
                                    {"Authenticate"}
                                </button>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // GPodder Section - hide completely when internal API is enabled
            {
                if !should_hide_sync_options {
                    html! {
                        {
                            if !*is_internal_gpodder_enabled {
                                html! {
                                    <div class="mb-6 p-4 border rounded-lg">
                                        <h3 class="item_container-text text-md font-bold mb-4">{"GPodder-compatible Server"}</h3>
                                        <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
                                            <div>
                                                <label for="gpodder_url" class="block text-sm font-medium mb-2">{"Server URL"}</label>
                                                <input
                                                    type="text"
                                                    id="gpodder_url"
                                                    oninput={on_server_url_change}
                                                    class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
                                                    placeholder="https://mypodcastsync.mydomain.com"
                                                />
                                            </div>
                                            <div>
                                                <label for="gpodder_username" class="block text-sm font-medium mb-2">{"Username"}</label>
                                                <input
                                                    type="text"
                                                    id="gpodder_username"
                                                    oninput={on_username_change}
                                                    class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
                                                    placeholder="myusername"
                                                />
                                            </div>
                                            <div>
                                                <label for="gpodder_password" class="block text-sm font-medium mb-2">{"Password"}</label>
                                                <input
                                                    type="password"
                                                    id="gpodder_password"
                                                    oninput={on_password_change}
                                                    class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
                                                    placeholder="mypassword"
                                                />
                                            </div>
                                        </div>

                                        <div class="mt-4 flex space-x-4">
                                            <button
                                                onclick={on_test_connection}
                                                disabled={*is_testing_connection}
                                                class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                                            >
                                                {
                                                    if *is_testing_connection {
                                                        html! { <span class="flex items-center"><i class="ph ph-spinner animate-spin mr-2"></i>{"Testing..."}</span> }
                                                    } else {
                                                        html! { <span class="flex items-center"><i class="ph ph-check-circle mr-2"></i>{"Test Connection"}</span> }
                                                    }
                                                }
                                            </button>

                                            <button
                                                onclick={on_authenticate_server_click}
                                                class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                                            >
                                                {"Authenticate"}
                                            </button>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    }
                } else {
                    html! {}
                }
            }

            // {
            //     if *is_internal_gpodder_enabled {
            //         html! {
            //             <div class="mb-6 p-4 border rounded-lg bg-gray-50 dark:bg-gray-800">
            //                 <div class="flex items-center">
            //                     <i class="ph ph-info text-blue-500 mr-2 text-lg"></i>
            //                     <p class="text-sm">
            //                         {"External sync options (Nextcloud and GPodder server) are hidden while the internal gpodder API is enabled. Disable the internal API to configure external sync options."}
            //                     </p>
            //                     <p class="text-sm">
            //                         {"Or just keep using internal sync. It's more convienient and means you don't need to maintain another external server :)"}
            //                     </p>
            //                 </div>
            //             </div>
            //         }
            //     } else {
            //         html! {}
            //     }
            // }

            {
                if should_hide_sync_options || (determine_sync_type() == "external_gpodder" && *is_sync_configured) {
                    let current_sync_type = determine_sync_type();
                    html! {
                        <div class="mb-6 p-4 border rounded-lg bg-gray-50 dark:bg-gray-800">
                            <div class="flex items-center mb-2">
                                <i class="ph ph-info text-blue-500 mr-2 text-lg"></i>
                                <h4 class="font-medium">
                                    {
                                        match current_sync_type {
                                            "internal_gpodder" => "Internal gpodder API is active",
                                            "nextcloud" => "About Nextcloud Sync",
                                            "external_gpodder" => "About External GPodder Sync",
                                            _ => "Sync Information"
                                        }
                                    }
                                </h4>
                            </div>
                            <div class="ml-6">
                                {
                                    match current_sync_type {
                                        "internal_gpodder" => html! {
                                            <p class="text-sm">
                                                {"External sync options (Nextcloud and GPodder server) are hidden while the internal gpodder API is enabled. Disable the internal API to configure an external sync option."}
                                            </p>
                                        },
                                        "nextcloud" => html! {
                                            <>
                                                <p class="text-sm mb-2">
                                                    {"Nextcloud sync is currently active. After enabling, it can take up to 20 minutes to fully synchronize all your podcasts."}
                                                </p>
                                                <p class="text-sm mb-2">
                                                    {"Please note that Nextcloud sync is a more limited gpodder implementation compared to internal sync or a dedicated gpodder server. It lacks device management capabilities available in more advanced sync options."}
                                                </p>
                                                <p class="text-sm mb-2">
                                                    {"Nextcloud sync works well with AntennaPod on Android and other gpodder-compatible clients but has fewer configuration options."}
                                                </p>
                                                <p class="text-sm italic">
                                                    {"If you need more advanced sync features (like device management), consider using the internal gpodder API or a dedicated gpodder server instead."}
                                                </p>
                                            </>
                                        },
                                        "external_gpodder" => html! {
                                            <>
                                                <p class="text-sm mb-2">
                                                    {"External GPodder sync is currently active with "}<span class="font-medium">{(*nextcloud_url).clone()}</span>{"."}
                                                </p>
                                                <p class="text-sm mb-2">
                                                    {"GPodder sync provides full podcast synchronization capabilities including managing multiple devices, subscription synchronization, and episode status tracking."}
                                                </p>
                                                <p class="text-sm mb-2">
                                                    {"You can use this sync method with any gpodder-compatible clients like AntennaPod, and others."}
                                                </p>
                                                {
                                                    if *is_sync_configured && *sync_type == "gpodder" {
                                                        html! {
                                                            <p class="text-sm mt-3">
                                                                {"You can access advanced device management options by clicking the 'Show Extra Options' button below."}
                                                            </p>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </>
                                        },
                                        _ => html! {
                                            <p class="text-sm">
                                                {"No sync method is currently configured. You can choose from internal gpodder API, Nextcloud, or an external GPodder server."}
                                            </p>
                                        }
                                    }
                                }
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            {
                // Show advanced options toggle only if sync is configured and the type is gpodder
                if *is_sync_configured && *sync_type == "gpodder" {
                    html! {
                        <div class="mt-6">
                            <button
                                onclick={on_toggle_advanced}
                                class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                            >
                                {
                                    if *show_advanced_options {
                                        html! { <span class="flex items-center"><i class="ph ph-caret-up mr-2"></i>{"Hide Extra Options"}</span> }
                                    } else {
                                        html! { <span class="flex items-center"><i class="ph ph-caret-down mr-2"></i>{"Show Extra Options"}</span> }
                                    }
                                }
                            </button>

                            {
                                if *show_advanced_options {
                                    html! { <GpodderAdvancedOptions /> }
                                } else {
                                    html! {}
                                }
                            }
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
