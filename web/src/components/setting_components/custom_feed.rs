use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::call_add_custom_feed;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[function_component(CustomFeed)]
pub fn custom_feed() -> Html {
    let (i18n, _) = use_translation();
    let feed_url = use_state(|| "".to_string());
    let youtube_url = use_state(|| "".to_string());
    let (_state, dispatch) = use_store::<AppState>();
    let pod_user = use_state(|| "".to_string());
    let pod_pass = use_state(|| "".to_string());
    let is_loading = use_state(|| false);

    // API key, server name, and other data can be fetched from AppState if required
    let (state, _) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    // Correct setup for `on_password_change`
    let update_feed = {
        let feed_url = feed_url.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            feed_url.set(input.value());
        })
    };
    let update_pod_user = {
        let pod_user = pod_user.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            pod_user.set(input.value());
        })
    };
    let update_pod_pass = {
        let pod_pass = pod_pass.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            pod_pass.set(input.value());
        })
    };

    let update_youtube_url = {
        let youtube_url = youtube_url.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            youtube_url.set(input.value());
        })
    };

    // Podcast feed addition callback
    let custom_loading = is_loading.clone();
    let add_custom_feed = {
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let user_id = user_id;
        let feed_url = (*feed_url).clone();
        let dispatch = dispatch.clone();
        let is_loading_call = custom_loading.clone();
        // Capture translated messages before move
        let success_msg = i18n.t("custom_feed.podcast_successfully_added").to_string();
        let error_prefix = i18n.t("custom_feed.failed_to_add_podcast").to_string();

        Callback::from(move |_| {
            let success_msg = success_msg.clone();
            let error_prefix = error_prefix.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let feed_url = feed_url.clone();
            let dispatch = dispatch.clone();
            is_loading_call.set(true);
            let is_loading_wasm = is_loading_call.clone();
            let unstate_pod_user = (*pod_user).clone();
            let unstate_pod_pass = (*pod_pass).clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_add_custom_feed(
                    &server_name,
                    &feed_url,
                    &user_id.unwrap(),
                    &api_key.unwrap(),
                    Some(unstate_pod_user),
                    Some(unstate_pod_pass),
                    Some(false), // Not a YouTube channel
                    Some(30),
                )
                .await
                {
                    Ok(_) => {
                        // Update global state with success message
                        dispatch.reduce_mut(|state| {
                            state.info_message = Some(success_msg.clone());
                        });
                    }
                    Err(e) => {
                        // Format error message if you have a formatter function like in StartPageOptions
                        let formatted_error = format_error_message(&e.to_string());

                        // Update global state with error message
                        dispatch.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("{}{}", error_prefix.clone(), formatted_error));
                        });
                    }
                }
                is_loading_wasm.set(false);
            });
        })
    };

    // YouTube channel addition callback
    let youtube_loading = is_loading.clone();
    let add_youtube_channel = {
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let user_id = user_id;
        let youtube_url = (*youtube_url).clone();
        let dispatch = dispatch.clone();
        let is_loading_call = youtube_loading.clone();
        // Capture translated messages before move
        let success_msg = i18n.t("custom_feed.podcast_successfully_added").to_string();
        let error_prefix = i18n.t("custom_feed.failed_to_add_podcast").to_string();

        Callback::from(move |_| {
            let success_msg = success_msg.clone();
            let error_prefix = error_prefix.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let youtube_url = youtube_url.clone();
            let dispatch = dispatch.clone();
            is_loading_call.set(true);
            let is_loading_wasm = is_loading_call.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_add_custom_feed(
                    &server_name,
                    &youtube_url,
                    &user_id.unwrap(),
                    &api_key.unwrap(),
                    None, // No username for YouTube
                    None, // No password for YouTube
                    Some(true), // IS a YouTube channel
                    Some(30),
                )
                .await
                {
                    Ok(_) => {
                        // Update global state with success message
                        dispatch.reduce_mut(|state| {
                            state.info_message = Some(success_msg.clone());
                        });
                    }
                    Err(e) => {
                        // Format error message if you have a formatter function like in StartPageOptions
                        let formatted_error = format_error_message(&e.to_string());

                        // Update global state with error message
                        dispatch.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("{}{}", error_prefix.clone(), formatted_error));
                        });
                    }
                }
                is_loading_wasm.set(false);
            });
        })
    };

    html! {
        <div class="p-4">
            <p class="item_container-text text-lg font-bold mb-4">{i18n.t("custom_feed.add_feed_title")}</p>
            <p class="item_container-text text-md mb-4">{i18n.t("custom_feed.add_feed_description")}</p>

            <br/>
            // Podcast Feed Section
            <div>
                <div>
                    <input id="feed_url" oninput={update_feed.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder={i18n.t("custom_feed.feed_url_placeholder")} />
                </div>
                <div>
                    <input id="username" oninput={update_pod_user.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5 mt-2" placeholder={i18n.t("custom_feed.username_optional")} />
                </div>
                <div>
                    <input id="password" type="password" oninput={update_pod_pass.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5 mt-2" placeholder={i18n.t("custom_feed.password_optional")} />
                </div>
                <button onclick={add_custom_feed} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" disabled={*is_loading}>
                {i18n.t("custom_feed.add_feed")}
                if *is_loading {
                    <span class="ml-2 spinner-border animate-spin inline-block w-4 h-4 border-2 rounded-full"></span>
                }
                </button>
            </div>

            <hr class="my-4 border-t"/>

            // YouTube Channel Section
            <div>
                <p class="item_container-text text-sm mb-2">{i18n.t("podcasts.youtube_channel_instructions")}</p>
                <div>
                    <input id="youtube_url" oninput={update_youtube_url.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder={i18n.t("podcasts.youtube_channel_url_placeholder")} />
                </div>
                <button onclick={add_youtube_channel} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" disabled={*is_loading}>
                {i18n.t("podcasts.add_channel")}
                if *is_loading {
                    <span class="ml-2 spinner-border animate-spin inline-block w-4 h-4 border-2 rounded-full"></span>
                }
                </button>
            </div>
        </div>
    }
}
