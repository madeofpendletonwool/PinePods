use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use crate::requests::setting_reqs::call_add_custom_feed;
use web_sys::HtmlInputElement;
use gloo_timers::callback::Timeout;

#[function_component(CustomFeed)]
pub fn custom_feed() -> Html {
    let feed_url = use_state(|| "".to_string());
    let error_message = use_state(|| None::<String>);
    let info_message = use_state(|| None::<String>);

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
    // Function to clear message
    let clear_error = {
        let error_message = error_message.clone();
        Callback::from(move |_| {
            error_message.set(None);
        })
    };

    let clear_info = {
        let info_message = info_message.clone();
        Callback::from(move |_| {
            info_message.set(None);
        })
    };

    // Ensure `onclick_restore` is correctly used
    let add_custom_feed = {
        let api_key = api_key.unwrap_or_default();
        let server_name = server_name.unwrap_or_default();
        let user_id = user_id;
        let feed_url = (*feed_url).clone();
        let error_message = error_message.clone();
        let info_message = info_message.clone();
        let clear_info = clear_info.clone();
        let clear_error = clear_error.clone();
        Callback::from(move |_| {
            let clear_info = clear_info.clone();
            let clear_error = clear_error.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let feed_url = feed_url.clone();
            let error_message = error_message.clone();
            let info_message = info_message.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_add_custom_feed(&server_name, &feed_url, &user_id.unwrap(), &api_key.unwrap()).await {
                    Ok(message) => {
                        info_message.set(Some(message));
                        Timeout::new(5000, move || { clear_info.emit(()) }).forget();
                    },
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                        Timeout::new(5000, move || { clear_error.emit(()) }).forget();
                    }
                }
            });
        })
    };

    html! {
        <div class="p-4">
            <p class="item_container-text text-lg font-bold mb-4">{"Add Feed:"}</p>
            <p class="item_container-text text-md mb-4">{"Use this to add a custom feed to your podcasts. Simply enter the feed url and click the button below. This is great in case you subscibe to premium podcasts and they aren't availble in The Pocast Index or other indexing services. After adding here, podcasts will show up and be available just like any others."}</p>
            
            <br/>
            <div>
                <div>
                    <input id="feed_url" oninput={update_feed.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder="https://bestpodcast.com/feed.xml" />
                </div>
                // Display error message inline right below the text input
                if let Some(error) = &*error_message {
                    <span class="text-red-600 text-xs">{ error }</span>
                }
                // Display informational message inline right below the text input
                if let Some(info) = &*info_message {
                    <span class="text-green-600 text-xs">{ info }</span>
                }
            </div>
            <button onclick={add_custom_feed} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
            {"Add Feed"}
            </button>
        </div>
    }
}
