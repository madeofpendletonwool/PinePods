use crate::components::context::{AppState, NotificationState};
use crate::requests::pod_req::{
    call_get_youtube_video_filter, call_update_youtube_video_filter, YouTubeVideoFilter,
};
use i18nrs::yew::use_translation;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Properties, PartialEq)]
pub struct YouTubeVideoFilterProps {
    pub podcast_id: i32,
}

fn minutes_text(minutes: Option<i32>) -> String {
    minutes.map(|m| m.to_string()).unwrap_or_default()
}

/// Settings for which new videos a YouTube channel subscription adds: shortest and longest
/// length, and text the title must contain. Empty fields mean no limit.
#[function_component(YouTubeVideoFilterSettings)]
pub fn youtube_video_filter_settings(props: &YouTubeVideoFilterProps) -> Html {
    let (app_state, _) = use_store::<AppState>();
    let (i18n, _) = use_translation();
    let auth = app_state
        .auth_details
        .as_ref()
        .map(|d| (d.server_name.clone(), d.api_key.clone().unwrap_or_default()));
    let user_id = app_state.user_details.as_ref().map(|d| d.UserID);

    let min_input = use_state(String::new);
    let max_input = use_state(String::new);
    let title_input = use_state(String::new);

    let show = {
        let (min_input, max_input, title_input) = (min_input.clone(), max_input.clone(), title_input.clone());
        move |filter: &YouTubeVideoFilter| {
            min_input.set(minutes_text(filter.min_minutes));
            max_input.set(minutes_text(filter.max_minutes));
            title_input.set(filter.title_contains.clone().unwrap_or_default());
        }
    };

    {
        let (auth, show) = (auth.clone(), show.clone());
        use_effect_with((props.podcast_id, user_id), move |(podcast_id, user_id)| {
            if let (Some((server, key)), Some(user_id)) = (auth, *user_id) {
                let podcast_id = *podcast_id;
                spawn_local(async move {
                    match call_get_youtube_video_filter(&server, &key, podcast_id, user_id).await {
                        Ok(filter) => show(&filter),
                        Err(e) => web_sys::console::log_1(&format!("Error getting video filter: {}", e).into()),
                    }
                });
            }
            || ()
        });
    }

    let on_input = |state: UseStateHandle<String>| {
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                state.set(input.value());
            }
        })
    };

    let save = {
        let (min_input, max_input, title_input) = (min_input.clone(), max_input.clone(), title_input.clone());
        let podcast_id = props.podcast_id;
        let saved_msg = i18n.t("episodes_layout.youtube_filter_saved").to_string();
        let error_msg = i18n.t("episodes_layout.youtube_filter_error").to_string();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let (Some((server, key)), Some(user_id)) = (auth.clone(), user_id) else { return };
            let filter = YouTubeVideoFilter {
                min_minutes: min_input.trim().parse().ok(),
                max_minutes: max_input.trim().parse().ok(),
                title_contains: Some(title_input.trim().to_string()).filter(|t| !t.is_empty()),
            };
            let (show, saved_msg, error_msg) = (show.clone(), saved_msg.clone(), error_msg.clone());
            spawn_local(async move {
                match call_update_youtube_video_filter(&server, &key, podcast_id, user_id, &filter).await {
                    Ok(stored) => {
                        show(&stored);
                        Dispatch::<NotificationState>::global()
                            .reduce_mut(|state| state.info_message = Some(saved_msg));
                    }
                    Err(e) => {
                        Dispatch::<NotificationState>::global()
                            .reduce_mut(|state| state.error_message = Some(format!("{}: {}", error_msg, e)));
                    }
                }
            });
        })
    };

    html! {
        <div class="mt-4">
            <label class="block mb-2 text-sm font-medium">{ i18n.t("episodes_layout.youtube_video_filter") }</label>
            <div class="flex flex-wrap items-end gap-2">
                <div>
                    <label for="yt-min-minutes" class="block mb-1 text-xs">{ i18n.t("episodes_layout.youtube_min_minutes") }</label>
                    <input type="number" id="yt-min-minutes" min="0"
                        class="email-input border text-sm rounded-lg p-2.5 w-24"
                        value={(*min_input).clone()} oninput={on_input(min_input.clone())} />
                </div>
                <div>
                    <label for="yt-max-minutes" class="block mb-1 text-xs">{ i18n.t("episodes_layout.youtube_max_minutes") }</label>
                    <input type="number" id="yt-max-minutes" min="0"
                        class="email-input border text-sm rounded-lg p-2.5 w-24"
                        value={(*max_input).clone()} oninput={on_input(max_input.clone())} />
                </div>
                <div class="flex-1 min-w-[10rem]">
                    <label for="yt-title-contains" class="block mb-1 text-xs">{ i18n.t("episodes_layout.youtube_title_contains") }</label>
                    <input type="text" id="yt-title-contains" maxlength="255"
                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                        placeholder={i18n.t("episodes_layout.youtube_title_placeholder")}
                        value={(*title_input).clone()} oninput={on_input(title_input.clone())} />
                </div>
                <button class="download-button font-bold py-2 px-4 rounded" onclick={save}>
                    { i18n.t("episodes_layout.save") }
                </button>
            </div>
            <p class="text-xs text-gray-500 mt-1">{ i18n.t("episodes_layout.youtube_filter_description") }</p>
        </div>
    }
}
